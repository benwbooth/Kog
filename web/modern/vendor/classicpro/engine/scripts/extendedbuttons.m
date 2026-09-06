#include <lib/std.mi>
#include <lib/pldir.mi>
#include "../lib/ClassicProFile.mi"

Function String getMyPath();

Global Group myGroup;
Global Button prevBut, nextBut, openBut;

System.onScriptLoaded() {
	myGroup = getScriptGroup();
	
	prevBut = myGroup.findObject(getToken(getParam(), ";", 0));
	nextBut = myGroup.findObject(getToken(getParam(), ";", 1));
	openBut = myGroup.findObject(getToken(getParam(), ";", 2));
}

openBut.onLeftClick(){
	if(System.isKeyDown(VK_ALT)){
		String curFile = getPlayItemMetaDataString("filename");
		String curPath = getMyPath();
		List returnValueStorage;
		returnValueStorage = new List;
		int error = ClassicProFile.findFiles(curPath, "*."+getExtension(getPlayItemString()), returnValueStorage);
		if(error==-1) return;

		PlEdit.clear();
		String enqFile = "";
		for(int i = 0; i<returnValueStorage.getNumItems();i++){
			enqFile = curPath + returnValueStorage.enumItem(i);
			PlEdit.enqueueFile(enqFile);
			if(curFile==enqFile) PlEdit.playTrack(i);
		}
		complete;
	}
}

String getMyPath(){
	String bs = strleft("\ ",1);
	String output = "";

	if(System.strleft(System.getPlayItemString(),6) == "cda://") output = System.strmid(System.getPlayItemString(), 6, 1)+":"+bs;
	else output= getPath(getPlayItemMetaDataString("filename"))+bs;
	return output;
}

nextBut.onLeftClick(){
	if(System.isKeyDown(VK_ALT)){
		String curFile = getPlayItemMetaDataString("filename");
		String curPath = getMyPath();
		List returnValueStorage;
		returnValueStorage = new List;
		int error = ClassicProFile.findFiles(curPath, "*.*", returnValueStorage);
		if(error==-1) return;

		String test = "";
		PlEdit.clear();
		String enqFile = "";
		for(int i = 0; i<returnValueStorage.getNumItems();i++){
			enqFile = curPath + returnValueStorage.enumItem(i);

			if(curFile==enqFile){
				if(i==returnValueStorage.getNumItems()-1){
					PlEdit.enqueueFile(curPath + returnValueStorage.enumItem(2));
				}
				else PlEdit.enqueueFile(curPath + returnValueStorage.enumItem(i+1));
				PlEdit.playTrack(0);
				return;
			}
		}
		complete;
	}
}

prevBut.onLeftClick(){
	if(System.isKeyDown(VK_ALT)){
		String curFile = getPlayItemMetaDataString("filename");
		String curPath = getMyPath();
		List returnValueStorage;
		returnValueStorage = new List;
		int error = ClassicProFile.findFiles(curPath, "*.*", returnValueStorage);
		if(error==-1) return;

		PlEdit.clear();
		String enqFile = "";
		for(int i = 0; i<returnValueStorage.getNumItems();i++){
			enqFile = curPath + returnValueStorage.enumItem(i);

			if(curFile==enqFile){
				if(i<3){
					PlEdit.enqueueFile(curPath + returnValueStorage.enumItem(returnValueStorage.getNumItems()-1));
				}
				else{
					PlEdit.enqueueFile(curPath + returnValueStorage.enumItem(i-1));
				}
				PlEdit.playTrack(0);
				return;
			}
		}
		complete;
	}
}
